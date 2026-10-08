//! Data model: the editable [`Terrain`] and the built [`TerrainSurface`].

use std::sync::OnceLock;

use plan_core::Point;
use serde::{Deserialize, Serialize};

use crate::landscape::{Landscape, ObjectStyle, TerrainBreak, TerrainWall};
use crate::query::TriIndex;

/// A spot height (Elevation Point tool).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ElevationPoint {
    pub pos: Point,
    /// Elevation, inches.
    pub z: f64,
}

/// Tension of a cardinal spline: 0.5 is Catmull-Rom, 0 a polyline.
pub const DEFAULT_TENSION: f64 = 0.5;

/// A polyline at one height (Elevation Line / Elevation Spline once flattened).
///
/// An Elevation Spline keeps its clicked `control` points and the `tension` it
/// was flattened with, so the curve can be re-flattened when the tension is
/// edited ([`ElevationLine::reflatten`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ElevationLine {
    pub points: Vec<Point>,
    pub z: f64,
    /// Control points of a spline line (empty for a plain polyline).
    pub control: Vec<Point>,
    /// Spline tension, 0 (straight segments) to 1 (loose); 0.5 is Catmull-Rom.
    pub tension: f64,
}

impl Default for ElevationLine {
    fn default() -> Self {
        ElevationLine {
            points: Vec::new(),
            z: 0.0,
            control: Vec::new(),
            tension: DEFAULT_TENSION,
        }
    }
}

impl ElevationLine {
    /// A straight line at `z`.
    pub fn polyline(points: Vec<Point>, z: f64) -> Self {
        ElevationLine {
            points,
            z,
            ..ElevationLine::default()
        }
    }

    /// A spline through `control` at `z` with `tension` (`samples` points per span).
    pub fn spline(control: Vec<Point>, z: f64, tension: f64, samples: usize) -> Self {
        let points = crate::geom::flatten_spline_tension(&control, false, samples, tension);
        ElevationLine {
            points,
            z,
            control,
            tension,
        }
    }

    /// Rebuilds `points` from the control points with the current tension
    /// (does nothing for a plain polyline).
    pub fn reflatten(&mut self, samples: usize) {
        if self.control.len() >= 3 {
            self.points =
                crate::geom::flatten_spline_tension(&self.control, false, samples, self.tension);
        }
    }
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

/// Default slope ratio of the sloped sides of a graded pad: 2 horizontal per
/// 1 vertical (1:2).
pub const DEFAULT_SLOPE_RATIO: f64 = 2.0;

/// A feature overlay. A `Hole` cuts the surface away. A `pad` feature is true
/// cut and fill: the terrain under the outline is levelled to a flat pad and
/// its sides slope back to the existing ground at `slope_ratio`. Any other
/// feature is a flat slab over the ground ([`crate::landscape_meshes`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Feature {
    pub kind: FeatureKind,
    pub polygon: Vec<Point>,
    pub material: String,
    /// Height of the flat top above the mean ground under the outline, inches
    /// (negative cuts the pad into the ground).
    pub height: f64,
    /// Layer, line style and fill style.
    pub style: ObjectStyle,
    /// Grade the terrain to this feature (cut/fill pad with sloped sides).
    pub pad: bool,
    /// Run per unit of rise of the pad's sloped sides (2 = 1:2).
    pub slope_ratio: f64,
    /// Control points of a kidney or spline outline (the polygon is the
    /// closed spline through them); empty when the outline was clicked point
    /// by point.
    pub control: Vec<Point>,
}

impl Default for Feature {
    fn default() -> Self {
        Feature {
            kind: FeatureKind::Rectangular,
            polygon: Vec::new(),
            material: String::new(),
            height: 0.0,
            style: ObjectStyle::default(),
            pad: false,
            slope_ratio: DEFAULT_SLOPE_RATIO,
            control: Vec::new(),
        }
    }
}

impl Feature {
    /// Rebuilds the outline from the control points (the closed spline through
    /// them). Does nothing for an outline without control points.
    pub fn reflatten(&mut self) {
        if self.control.len() >= 3 {
            self.polygon = crate::landscape::closed_spline(&self.control);
        }
    }
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
#[serde(default)]
pub struct RoadStrip {
    pub kind: RoadKind,
    pub centerline: Vec<Point>,
    /// Full strip width, inches.
    pub width: f64,
    pub curb: bool,
    /// How much higher the centerline sits than the edges (a crowned road
    /// sheds water to both curbs), inches.
    pub crown: f64,
    /// Height of the curb blocks, inches.
    pub curb_height: f64,
}

impl Default for RoadStrip {
    fn default() -> Self {
        RoadStrip {
            kind: RoadKind::Road,
            centerline: Vec::new(),
            width: 240.0,
            curb: false,
            crown: 0.0,
            curb_height: 6.0,
        }
    }
}

/// The building pad: the terrain under the house is levelled to the first
/// floor less the plan's "terrain to first floor" distance
/// ([`Terrain::subfloor_height_above_terrain`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BuildingPad {
    /// The building outline (foundation or first floor walls).
    pub footprint: Vec<Point>,
    /// How far the level pad reaches beyond the footprint, inches.
    pub margin: f64,
    /// Elevation of the first floor, inches. `None` levels the pad at the mean
    /// existing ground under it (a balanced cut and fill).
    pub first_floor: Option<f64>,
    /// Run per unit of rise of the pad's sloped sides.
    pub slope_ratio: f64,
}

impl Default for BuildingPad {
    fn default() -> Self {
        BuildingPad {
            footprint: Vec::new(),
            margin: 24.0,
            first_floor: None,
            slope_ratio: DEFAULT_SLOPE_RATIO,
        }
    }
}

/// Every this-many'th contour is a major contour.
pub const DEFAULT_MAJOR_EVERY: u32 = 5;
/// Default distance between elevation labels along a major contour, inches.
pub const DEFAULT_LABEL_SPACING: f64 = 480.0;

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
    /// Terrain Break lines: flat creases that stay sharp in the surface.
    pub breaks: Vec<TerrainBreak>,
    /// Terrain walls and curbs (retaining walls that follow the ground).
    pub walls: Vec<TerrainWall>,
    /// Garden beds, grass, water, stepping stones, plant runs and sprinklers.
    pub landscape: Vec<Landscape>,
    /// Chief default 6": how far the subfloor sits above the terrain at the building.
    pub subfloor_height_above_terrain: f64,
    /// Elevation of the building pad, inches.
    pub building_pad_elevation: f64,
    /// Laplacian smoothing iterations applied to interior vertices (default 0).
    pub smoothing: u32,
    /// Spacing of the sample grid, inches (default 120").
    pub grid_spacing: f64,
    /// The sample grid is divided this many times finer (1 = as is).
    pub subdivision: u32,
    /// Every this-many'th contour is a major contour.
    pub contour_major_every: u32,
    /// Distance between elevation labels along a contour, inches (0 = one
    /// label per contour line).
    pub contour_label_spacing: f64,
    /// Label only the major contours.
    pub contour_label_major_only: bool,
    /// The building pad, levelled under the house.
    pub building_pad: Option<BuildingPad>,
    /// Level the building pad (when one is defined).
    pub flatten_pad: bool,
    /// Compass rotation of the plan: degrees clockwise from the top of the
    /// plan to true north (the North Pointer). The sun angle and the compass
    /// labels use it.
    pub north_angle: f64,
    /// The placed North Pointer (its CAD objects on the Site Plan layer).
    pub north_pointer: Option<crate::site_symbols::SiteMark>,
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
            breaks: Vec::new(),
            walls: Vec::new(),
            landscape: Vec::new(),
            subfloor_height_above_terrain: 6.0,
            building_pad_elevation: 0.0,
            smoothing: 0,
            grid_spacing: 120.0,
            subdivision: 1,
            contour_major_every: DEFAULT_MAJOR_EVERY,
            contour_label_spacing: DEFAULT_LABEL_SPACING,
            contour_label_major_only: true,
            building_pad: None,
            flatten_pad: true,
            north_angle: 0.0,
            north_pointer: None,
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
